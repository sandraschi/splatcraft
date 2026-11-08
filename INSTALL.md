# Installing splatcraft

Windows x64 is the only tested platform. Pick the path that fits; Option A needs no manual steps.

## Option A: tell Claude to install it (easiest)

Open Claude Code, Claude Desktop with a shell/filesystem tool, or any coding agent and say:

> Install splatcraft from github.com/sandraschi/splatcraft

The agent should read this file and follow the "Agent install steps" section below. Before
v0.1.0 is published as a GitHub release, this builds from source, so it needs the Rust
toolchain (the agent installs it if you agree). Once a release exists, the agent prefers the
prebuilt zip and skips the build.

## Option B: prebuilt zip (after the first release is published)

1. Open [Releases](https://github.com/sandraschi/splatcraft/releases/latest).
2. Download `splatcraft-v*-windows-x64.zip` and `SHA256SUMS.txt`, verify the hash.
3. Unzip anywhere. `splatcraft.exe` is the desktop app, `splatcraft-cli.exe` is the CLI and
   MCP server (`splatcraft-cli.exe mcp`).

## Option C: MCPB drag and drop (Claude Desktop)

mcpb is not Python-only: its manifest supports `binary` servers, and splatcraft uses that.
The bundle wraps the Rust `splatcraft-cli.exe`, no Python or Node is involved at runtime.

1. Download `splatcraft-v*.mcpb` from the release page (or build it, see Option D).
2. In Claude Desktop settings, use the install-from-file option for extensions and pick the file (menu names vary by Desktop version).

Status: the bundle passes `mcpb validate` and packs, but installing it in Claude Desktop has
not been tested yet. If Desktop rejects it, use Option E and tell us.

## Option D: build from source

Needs Git, the Rust toolchain (rustup, stable, MSVC build tools) and, for `render` and the
app viewport, a GPU with a current driver.

```
git clone https://github.com/sandraschi/splatcraft
cd splatcraft
cargo build --release --workspace
```

Binaries land in `target\release\`. To build the zip and the `.mcpb` yourself:
`pwsh -NoProfile -File scripts\release.ps1` (builds only; it never tags or uploads without
`-Publish`).

## Option E: register the MCP server by hand

Claude Code:

```
claude mcp add splatcraft -- C:\path\to\splatcraft-cli.exe mcp
```

Claude Desktop, in `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "splatcraft": {
      "command": "C:\\path\\to\\splatcraft-cli.exe",
      "args": ["mcp"],
      "env": { "SPLATCRAFT_TOOLS": "C:\\path\\to\\tools" }
    }
  }
}
```

Restart Claude Desktop afterwards. `SPLATCRAFT_TOOLS` is only needed for training (below).

## External engines (only for `train`)

Everything except `train` works with the splatcraft binaries alone. Training needs three
external programs that are not bundled:

| Tool | Used for | Where |
|---|---|---|
| ffmpeg | frame extraction | any build on PATH (scoop, winget) |
| COLMAP 4.x | structure from motion | github.com/colmap/colmap releases (`colmap-x64-windows-cuda.zip` for NVIDIA, `-nocuda` otherwise) |
| Brush | the trainer (Rust) | github.com/ArthurBrussee/brush releases (`brush-app-x86_64-pc-windows-msvc.zip`) |

Unzip COLMAP and Brush into subfolders of one tools directory (for example
`brush\brush_app.exe`, `colmap\bin\colmap.exe`) and set `SPLATCRAFT_TOOLS` to that directory.
Per-tool overrides (`SPLATCRAFT_BRUSH`, `SPLATCRAFT_COLMAP`, `SPLATCRAFT_FFMPEG`) win over it.
Check with `splatcraft-cli engines`. Nerfstudio is detected but unused.

## Agent install steps

For an AI agent asked to install splatcraft. Follow in order, stop and report on any failure,
and do not improvise around errors.

1. Ask the user where to install (default `D:\Dev\repos\splatcraft` on this owner's machine,
   otherwise a folder of their choice) and whether they want training support (the external
   engines, roughly 550 MB). Do not download engines unless they say yes.
2. Check for a published release: `gh release view --repo sandraschi/splatcraft` or the
   GitHub releases page. If one exists, download the zip and `SHA256SUMS.txt`, verify the
   SHA-256, unzip, and skip to step 5. If none exists, continue to step 3.
3. Check prerequisites: `git --version`, `cargo --version`. If Rust is missing, tell the user
   and install it with rustup only after they agree. MSVC build tools are required on Windows.
4. `git clone https://github.com/sandraschi/splatcraft <dir>`, then in that directory
   `cargo build --release --workspace`. A first build takes a few minutes.
5. Register the MCP server (Option E) using the absolute path to `splatcraft-cli.exe`.
   Prefer `claude mcp add` in Claude Code; for Claude Desktop edit the config JSON, keep any
   existing servers, and tell the user to restart Desktop.
6. If the user wants training: download COLMAP and Brush from the release pages above, verify
   checksums where the project publishes them, unpack into a tools directory, and set
   `SPLATCRAFT_TOOLS`. Do not put them on PATH.
7. Verify: run `splatcraft-cli.exe status` and `splatcraft-cli.exe engines` and report which
   engines are up. Run `splatcraft.exe --smoke` if a GPU is expected.
8. Do not set or ask for `WORLDLABS_API_KEY` unless the user supplies it. Never run
   `worlds generate` with `confirm_spend` true; it costs real money.
9. Report exactly what was installed, where, and what was not verified.
