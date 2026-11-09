#Requires -Version 5.1
<#
.SYNOPSIS
    splatcraft release: gates, release build, smoke, exes + mcpb bundle, checksums.

.DESCRIPTION
    Rust adaptation of the fleet release template. There is no Tauri, no PyInstaller sidecar
    and no NSIS installer: the app is egui and ships as plain executables.
    By default this only BUILDS and STAGES artifacts in dist/. Nothing is tagged, pushed or
    uploaded unless -Publish is given. Exits at the first failed phase.

.PARAMETER SkipGates
    Skip fmt, clippy and tests (they must have been run just before).

.PARAMETER SkipMcpb
    Skip the mcpb bundle.

.PARAMETER Publish
    After a successful build: create the git tag, push it, create the GitHub release with
    CHANGELOG_LATEST.md as notes and upload the artifacts. Requires gh and a clean master.

.EXAMPLE
    pwsh -NoProfile -File scripts/release.ps1
    pwsh -NoProfile -File scripts/release.ps1 -Publish
#>
param(
    [switch]$SkipGates,
    [switch]$SkipMcpb,
    [switch]$Publish
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot
$RepoName = Split-Path -Leaf $RepoRoot
$Dist = Join-Path $RepoRoot 'dist'
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

function Write-Phase { param([string]$m) Write-Host ''; Write-Host "=== $m ===" -ForegroundColor Cyan }
function Write-Ok { param([string]$m) Write-Host "  [ok] $m" -ForegroundColor Green }
function Write-Info { param([string]$m) Write-Host "  [..] $m" -ForegroundColor Gray }
function Fail { param([string]$m) Write-Host "  [FAIL] $m" -ForegroundColor Red; throw $m }
function Need { param([int]$code, [string]$step) if ($code -ne 0) { Fail "$step exited with code $code" } }

# ---- Phase 0: preflight ------------------------------------------------------
Write-Phase 'Phase 0: preflight'
Set-Location $RepoRoot
$cargo = Get-Content (Join-Path $RepoRoot 'Cargo.toml') -Raw
if ($cargo -notmatch '(?ms)\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') { Fail 'workspace version not found in Cargo.toml' }
$Version = $Matches[1]
$Tag = "v$Version"
Write-Ok "Version $Version, tag $Tag"

$dirty = git status --porcelain
if ($dirty) { Fail "working tree is dirty:`n$dirty" }
Write-Ok 'tree clean'

$branch = (git rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne 'master') { Fail "on branch '$branch', releases are cut from master" }
$extra = git branch --format='%(refname:short)' | Where-Object { $_ -ne 'master' }
if ($extra) { Fail "extra local branches exist: $($extra -join ', '). Merge or delete them first (see AGENTS.md)" }
Write-Ok 'on master, no stray branches'

if (git tag --list $Tag) { Fail "tag $Tag already exists locally; bump the workspace version" }
if ($Publish) {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { Fail 'gh CLI not found' }
    if (git ls-remote --tags origin "refs/tags/$Tag") { Fail "tag $Tag already exists on the remote" }
    # git writes progress to stderr; under Windows PowerShell 5.1 with EAP=Stop a 2>&1 redirect
    # turns that into a terminating error, so relax EAP for this call and check the exit code.
    $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    git fetch -q origin master 2>$null; $fetchCode = $LASTEXITCODE
    $ErrorActionPreference = $prevEap
    Need $fetchCode 'git fetch origin master'
    $ahead = git rev-list --count 'origin/master..HEAD'
    if ($ahead -ne '0') { Fail "$ahead local commit(s) not pushed; push master first" }
}
if (-not (Test-Path (Join-Path $RepoRoot 'CHANGELOG_LATEST.md'))) { Fail 'CHANGELOG_LATEST.md missing' }
Write-Ok 'preflight done'

# ---- Phase 1: gates ----------------------------------------------------------
if (-not $SkipGates) {
    Write-Phase 'Phase 1: gates'
    cargo fmt --all --check; Need $LASTEXITCODE 'fmt'
    cargo clippy --workspace --all-targets --all-features -- -D warnings; Need $LASTEXITCODE 'clippy'
    cargo test --workspace; Need $LASTEXITCODE 'tests'
    Write-Ok 'fmt, clippy, tests green'
}

# ---- Phase 2: release build + smoke -----------------------------------------
Write-Phase 'Phase 2: release build and smoke'
cargo build --release --workspace; Need $LASTEXITCODE 'release build'
$app = Join-Path $RepoRoot 'target\release\splatcraft.exe'
$cli = Join-Path $RepoRoot 'target\release\splatcraft-cli.exe'
foreach ($f in $app, $cli) { if (-not (Test-Path $f)) { Fail "missing build output: $f" } }
& $app --smoke; Need $LASTEXITCODE 'splatcraft --smoke'
& $cli status | Out-Null; Need $LASTEXITCODE 'splatcraft-cli status'
$init = '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"release-smoke","version":"0"}}}'
$resp = $init | & $cli mcp | Select-Object -First 1
if ($resp -notmatch '"result"') { Fail "MCP initialize returned no result: $resp" }
Write-Ok 'smoke: app, cli status, MCP initialize'

# ---- Phase 3: stage artifacts -----------------------------------------------
Write-Phase 'Phase 3: artifacts'
if (Test-Path $Dist) { Remove-Item $Dist -Recurse -Force }
New-Item -ItemType Directory -Force $Dist | Out-Null
# Executables ship as plain release assets under stable names (no zip), so
# releases/latest/download/splatcraft-cli.exe is a fixed URL and MCP configs need no version.
$assets = @()
foreach ($f in $app, $cli) {
    $dst = Join-Path $Dist (Split-Path -Leaf $f)
    Copy-Item $f $dst
    Write-Ok "$(Split-Path -Leaf $f): $([math]::Round((Get-Item $dst).Length / 1MB, 1)) MB"
    $assets += $dst
}
if (-not $SkipMcpb) {
    $bunx = Join-Path $env:USERPROFILE '.bun\bin\bunx.exe'
    if (-not (Test-Path $bunx)) { $g = Get-Command bunx -ErrorAction SilentlyContinue; if ($g) { $bunx = $g.Source } else { Fail 'bunx not found' } }
    $mstage = Join-Path $Dist 'mcpb-stage'
    New-Item -ItemType Directory -Force $mstage | Out-Null
    $manifest = (Get-Content (Join-Path $RepoRoot 'mcpb\manifest.json') -Raw) -replace '__VERSION__', $Version
    [IO.File]::WriteAllText((Join-Path $mstage 'manifest.json'), $manifest, (New-Object Text.UTF8Encoding $false))
    Copy-Item $cli $mstage
    $mcpb = Join-Path $Dist "$RepoName-v$Version.mcpb"
    & $bunx '@anthropic-ai/mcpb' validate (Join-Path $mstage 'manifest.json'); Need $LASTEXITCODE 'mcpb validate manifest'
    & $bunx '@anthropic-ai/mcpb' pack $mstage $mcpb; Need $LASTEXITCODE 'mcpb pack'
    Remove-Item $mstage -Recurse -Force
    Write-Ok "mcpb: $([math]::Round((Get-Item $mcpb).Length / 1MB, 1)) MB"
    $assets += $mcpb
}

$sums = foreach ($a in $assets) { "{0}  {1}" -f (Get-FileHash $a -Algorithm SHA256).Hash.ToLower(), (Split-Path -Leaf $a) }
$sumFile = Join-Path $Dist 'SHA256SUMS.txt'
[IO.File]::WriteAllText($sumFile, (($sums -join "`n") + "`n"), (New-Object Text.UTF8Encoding $false))
$assets += $sumFile
$sums | ForEach-Object { Write-Info $_ }

# ---- Phase 4: publish (opt-in) ----------------------------------------------
if ($Publish) {
    Write-Phase "Phase 4: publish $Tag"
    git tag $Tag; Need $LASTEXITCODE 'git tag'
    git push origin $Tag; Need $LASTEXITCODE 'git push tag'
    gh release create $Tag --title "$RepoName $Tag" --notes-file (Join-Path $RepoRoot 'CHANGELOG_LATEST.md'); Need $LASTEXITCODE 'gh release create'
    foreach ($a in $assets) { gh release upload $Tag $a --clobber; Need $LASTEXITCODE "upload $(Split-Path -Leaf $a)" }
    Write-Ok "https://github.com/sandraschi/$RepoName/releases/tag/$Tag"
} else {
    Write-Phase 'Done (not published)'
    Write-Info "Artifacts are in $Dist. Re-run with -Publish to tag, push and upload."
}
