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
