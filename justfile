set windows-shell := ["powershell.exe", "-NoProfile", "-Command"]

# splatcraft automation (Pure-Rust ArtCraft pattern)

default:
    @just --list

# Strict lint gate across all workspace members
check:
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo clippy --workspace --all-targets --all-features -- -D warnings

# Unit + integration tests
test:
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo test --workspace

# Formatting gate
fmt-check:
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo fmt --all --check

# All gates in one command
ci: fmt-check check test

# MCP stdio server (a client normally starts this; useful for a manual probe)
run-mcp:
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo run -q -p splatcraft-mcp -- mcp

# CLI smoke: status then engine probe
run-cli:
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo run -q -p splatcraft-mcp -- status; cargo run -q -p splatcraft-mcp -- engines

# Production build
build-release:
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo build --release --workspace

# Headless check: app and CLI smoke from scripts/craft.config.psd1 (throwaway database).
smoke:
    powershell -NoProfile -ExecutionPolicy Bypass -File start.ps1 -Smoke

# Build and stage dist/ (plain exes, installer, launcher zip, SHA256SUMS). Nothing is published.
release-dry:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1

# Same, then tag, push the tag and upload to GitHub (clean, pushed trunk required).
release:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1 -Publish
