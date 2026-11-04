# fleet-start.config.ps1 for splatcraft
# Type: cli / stdio-mcp (no permanent webapp port needed)
@{
    RepoName = "splatcraft"
    Type = "cli"
    HealthCommand = "uv run splatcraft --version"
    StartCommand = "uv run splatcraft mcp"
}
