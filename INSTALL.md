# Installing splatcraft

Windows x64 is the only tested platform. Pick the path that fits; Option A needs no manual steps.

## Option A: tell Claude to install it (easiest)

Open any agentic IDE or CLI (Claude Code, Cursor, Antigravity, Zed, VS Code with Copilot agent mode,
Gemini CLI, Codex, OpenCode, Windsurf) and say:

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

## Option E: register the MCP server by hand (any agentic IDE)

Every host launches the same thing: `splatcraft-cli.exe` with the single argument `mcp`
(MCP over stdio). Only the config file and its shape differ. In the examples, replace
`C:/path/to/splatcraft-cli.exe` with the real absolute path (forward slashes work on Windows
and avoid JSON escaping). `SPLATCRAFT_TOOLS` is optional and only matters for `train`.
Restart or reload the host after editing, then check that the tools list shows
`splat_inspect`, `splat_train` and the rest.

| Host | Config file | Top-level key |
|---|---|---|
| Claude Code | managed by `claude mcp add` | n/a |
| Claude Desktop | `%APPDATA%\Claude\claude_desktop_config.json` (or the `.mcpb`, Option C) | `mcpServers` |
| Cursor | `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (project) | `mcpServers` |
| VS Code (Copilot agent mode) | `.vscode/mcp.json` (project) or the user `mcp.json` | `servers`, needs `"type": "stdio"` |
| Zed | `settings.json` | `context_servers`, `command` is an object |
| Antigravity (IDE, CLI, 2.0) | `~/.gemini/config/mcp_config.json` (global) or `.agents/mcp_config.json` (project) | `mcpServers` |
| Gemini CLI | `~/.gemini/settings.json` or `.gemini/settings.json` | `mcpServers` |
| Codex CLI / IDE extension | `~/.codex/config.toml` or `.codex/config.toml` | `[mcp_servers.<name>]` (TOML) |
| OpenCode | `opencode.json` in the project, or `~/.config/opencode/opencode.json` | `mcp`, `command` is an array |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` (legacy Cascade; docs have moved, verify) | `mcpServers` |

Hosts that follow the common `mcpServers` shape (Claude Desktop, Cursor, Antigravity, Gemini
CLI, Windsurf, Cline and most others):

```json
{
  "mcpServers": {
    "splatcraft": {
      "command": "C:/path/to/splatcraft-cli.exe",
      "args": ["mcp"],
      "env": { "SPLATCRAFT_TOOLS": "C:/path/to/tools" }
    }
  }
}
```

Claude Code:

```
claude mcp add splatcraft -- C:/path/to/splatcraft-cli.exe mcp
```

VS Code (`servers`, explicit `type`):

```json
{
  "servers": {
    "splatcraft": {
      "type": "stdio",
      "command": "C:/path/to/splatcraft-cli.exe",
      "args": ["mcp"]
    }
  }
}
```

Zed (`context_servers`; `command` is an object with `path`, `args`, `env`; no `type` field):

```json
{
  "context_servers": {
    "splatcraft": {
      "source": "custom",
      "command": {
        "path": "C:/path/to/splatcraft-cli.exe",
        "args": ["mcp"],
        "env": { "SPLATCRAFT_TOOLS": "C:/path/to/tools" }
      }
    }
  }
}
```

Codex CLI (`codex mcp add splatcraft -- C:/path/to/splatcraft-cli.exe mcp` also works):

```toml
[mcp_servers.splatcraft]
command = "C:/path/to/splatcraft-cli.exe"
args = ["mcp"]
env = { SPLATCRAFT_TOOLS = "C:/path/to/tools" }
```

OpenCode (`mcp` key, `type: "local"`, `command` is one array, env key is `environment`):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "splatcraft": {
      "type": "local",
      "command": ["C:/path/to/splatcraft-cli.exe", "mcp"],
      "enabled": true,
      "environment": { "SPLATCRAFT_TOOLS": "C:/path/to/tools" }
    }
  }
}
```

Notes: these formats were taken from each tool's current documentation, but hosts change
them often. If a host rejects the entry, read that host's own MCP docs rather than guessing,
and tell the owner. Antigravity docs currently give the global file as
`~/.gemini/config/mcp_config.json`; older setups used a folder under `~/.gemini/antigravity`,
so edit whichever file the host actually loads (its MCP manager can open the raw config).
Project-scoped configs (`.cursor`, `.vscode`, `.agents`, `.codex`, `opencode.json`) may need
approval or a trusted workspace before they load.

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

## API keys (only for `worlds`)

Marble world generation needs a World Labs API key. Store it once, yourself, in the Windows
Credential Manager:

```
splatcraft-cli secret set worlds
```

It prompts with hidden input, so the key never appears in a config file, a shell history or a
chat. `splatcraft-cli secret status` shows whether it is set and from where (never the value).
The environment variable `WORLDLABS_API_KEY` still overrides the stored key. Agents must not
ask for the key or paste it anywhere; see docs/SECRETS_STANDARD.md.

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
5. Register the MCP server using the absolute path to `splatcraft-cli.exe` and the host you
   are running in (Option E has the exact file and shape for Claude Code, Claude Desktop,
   Cursor, VS Code, Zed, Antigravity, Gemini CLI, Codex CLI, OpenCode and Windsurf). Use your own
   host's native mechanism, keep any existing servers in the file, and tell the user to
   restart or reload the host. If you are not sure which host you are in, ask.
6. If the user wants training: download COLMAP and Brush from the release pages above, verify
   checksums where the project publishes them, unpack into a tools directory, and set
   `SPLATCRAFT_TOOLS`. Do not put them on PATH.
7. Verify: run `splatcraft-cli.exe status` and `splatcraft-cli.exe engines` and report which
   engines are up. Run `splatcraft.exe --smoke` if a GPU is expected.
8. Never ask for, read or write a World Labs API key. If the user wants Marble, tell them to run
   `splatcraft-cli secret set worlds` themselves. Never run `worlds generate` with
   `confirm_spend` true; it costs real money.
9. Report exactly what was installed, where, and what was not verified.
