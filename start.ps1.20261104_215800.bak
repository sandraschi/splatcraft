# splatcraft launcher
param(
    [switch]$Mcp = $false,
    [Parameter(ValueFromRemainingArguments=$true)][string[]]$AppArgs
)
$ErrorActionPreference = "Stop"

Set-Location -LiteralPath $PSScriptRoot

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
}
