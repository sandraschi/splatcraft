default:
    @just --list

# Install dependencies with uv
install:
    uv sync

# Run test suite
test:
    uv run pytest -v tests/

# Format code with ruff
format:
    uv run ruff format .
    uv run ruff check --fix .

# Lint check
lint:
    uv run ruff check .
    uv run ruff format --check .

# Run CI checks
ci: lint test

# Run CLI
run *ARGS:
    uv run splatcraft {{ARGS}}

# Run stdio MCP server
mcp:
    uv run splatcraft mcp
