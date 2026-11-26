# `craft-secrets` (Lift-and-Share Secret Lookup)

Zero-splatcraft-dependency secret lookup crate shared across Craft apps: checks environment variables first, then falls back to the OS credential store (Windows Credential Manager). Secret values are never returned to MCP tools or logged.
