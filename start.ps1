# splatcraft launcher
param(
    [switch]$Mcp = $false,
    [Parameter(ValueFromRemainingArguments=$true)][string[]]$AppArgs
)
$ErrorActionPreference = "Stop"

# Ensure UTF-8 output and python encoding to prevent charmap crashes
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$env:PYTHONIOENCODING = "utf-8"
$env:PYTHONUTF8 = "1"

# Ensure user tools directory is in PATH
foreach ($toolDir in @("$env:USERPROFILE\.local\bin", "$env:USERPROFILE\.cargo\bin", "$env:USERPROFILE\.bun\bin")) {
    if ((Test-Path -LiteralPath $toolDir) -and ($env:PATH -split ';' -notcontains $toolDir)) {
        $env:PATH = "$toolDir;$env:PATH"
    }
}

Set-Location -LiteralPath $PSScriptRoot

# Verify uv is accessible
$uvCmd = Get-Command uv -ErrorAction SilentlyContinue
if (-not $uvCmd) {
    $fallbackUv = "$env:USERPROFILE\.local\bin\uv.exe"
    if (Test-Path -LiteralPath $fallbackUv) {
        $env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
    } else {
        Write-Host "ERROR: uv package manager not found. Install Astral uv: https://astral.sh/uv" -ForegroundColor Red
        exit 1
    }
}

# Auto-sync virtual environment if missing
if (-not (Test-Path -LiteralPath "$PSScriptRoot\.venv")) {
    Write-Host "[splatcraft] Virtual environment missing. Syncing dependencies (uv sync)..." -ForegroundColor Yellow
    & uv sync
}

if ($Mcp) {
    Write-Host "[splatcraft] Launching FastMCP stdio server..." -ForegroundColor Cyan
    & uv run splatcraft mcp
} elseif ($AppArgs -and $AppArgs.Count -gt 0) {
    & uv run splatcraft @AppArgs
} else {
    Write-Host "[splatcraft] Launching CLI help & status probe..." -ForegroundColor Cyan
    & uv run splatcraft --version
    & uv run splatcraft engines
    & uv run splatcraft --help

    # Interactive menu so launcher window never flash-closes on double-click
    while ($true) {
        Write-Host ""
        Write-Host "==================================================================" -ForegroundColor Cyan
        Write-Host "  SplatCraft 3D Gaussian Splat Studio (Interactive Console)       " -ForegroundColor Cyan
        Write-Host "==================================================================" -ForegroundColor Cyan
        Write-Host "  [1] Launch Stdio FastMCP Server"
        Write-Host "  [2] Probe 3DGS Training Engines (Nerfstudio / Postshot)"
        Write-Host "  [3] Run Democraft Recipes (generate test splats in outbox/)"
        Write-Host "  [4] Inspect a Splat (.ply or .spz)"
        Write-Host "  [5] Exit"
        Write-Host ""

        $raw = Read-Host "Choose option [1-5] or type command (or press Enter for MCP server)"
        if ($null -eq $raw) { break }
        $choice = $raw.Trim()
        if ($choice -eq "1" -or $choice -eq "" -or $choice -eq "mcp") {
            Write-Host "[splatcraft] Launching FastMCP stdio server..." -ForegroundColor Cyan
            & uv run splatcraft mcp
            break
        } elseif ($choice -eq "2" -or $choice -eq "engines") {
            & uv run splatcraft engines
        } elseif ($choice -eq "3" -or $choice -eq "recipe") {
            & uv run splatcraft recipe all
        } elseif ($choice -eq "4" -or $choice -eq "info") {
            $f = Read-Host "Enter path to .ply or .spz file"
            if ($f) {
                & uv run splatcraft info --file $f.Trim()
            }
        } elseif ($choice -eq "5" -or $choice -eq "exit" -or $choice -eq "q") {
            Write-Host "Exiting SplatCraft." -ForegroundColor DarkGray
            break
        } else {
            $argsList = -split $choice
            & uv run splatcraft @argsList
        }
    }
}
