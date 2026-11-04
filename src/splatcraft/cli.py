"""Command line interface for splatcraft."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Optional

import typer
from rich.console import Console
from rich.table import Table

from . import __version__
from .io import extract_collider, inspect_splat
from .server import splat_probe_local_engines

app = typer.Typer(
    name="splatcraft",
    help="3D Gaussian Splat & Radiance Field manipulation CLI.",
    add_completion=False,
)
console = Console()


@app.callback(invoke_without_command=True)
def main(
    version: Optional[bool] = typer.Option(None, "--version", "-v", help="Show version and exit"),
):
    if version:
        console.print(f"[bold cyan]splatcraft[/bold cyan] v{__version__}")
        raise typer.Exit()


@app.command()
def info(
    file: Path = typer.Option(..., "--file", "-f", help="Path to .ply or .spz file"),
    as_json: bool = typer.Option(False, "--json", help="Emit raw JSON for democraft / automation"),
):
    """Inspects a 3D Gaussian Splat and prints metadata."""
    try:
        world = inspect_splat(file)
    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
        raise typer.Exit(code=1)

    if as_json:
        console.print(world.model_dump_json(indent=2))
        return

    table = Table(title=f"Splatcraft Info: {world.name}")
    table.add_column("Property", style="cyan")
    table.add_column("Value", style="green")

    table.add_row("Format", world.format.upper())
    table.add_row("Splat Count", f"{world.splat_count:,}")
    table.add_row("SH Degree", str(world.sh_degree))
    table.add_row("Size (bytes)", f"{world.file_size_bytes:,}")
    table.add_row("BBox Min", str(world.bbox_min))
    table.add_row("BBox Max", str(world.bbox_max))

    console.print(table)


@app.command()
def collider(
    file: Path = typer.Option(..., "--file", "-f", help="Source splat file"),
    out: Path = typer.Option(..., "--out", "-o", help="Output collider path"),
    as_json: bool = typer.Option(False, "--json", help="Emit JSON output"),
):
    """Extracts bounding collider for gamecraft/Godot integration."""
    try:
        world = inspect_splat(file)
        bounds = extract_collider(world, out)
        if as_json:
            console.print(bounds.model_dump_json(indent=2))
        else:
            console.print(
                f"[bold green]Extracted collider:[/bold green] {out} (volume: {bounds.volume:.2f})"
            )
    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
        raise typer.Exit(code=1)


@app.command()
def engines(
    as_json: bool = typer.Option(False, "--json", help="Emit JSON status"),
):
    """Probes local host (RTX 4090) for Nerfstudio and Postshot training engines."""
    probe = splat_probe_local_engines()
    if as_json:
        console.print(json.dumps(probe, indent=2))
    else:
        table = Table(title="Local 3DGS Training Engines")
        table.add_column("Engine", style="cyan")
        table.add_column("Available", style="green")
        table.add_row("Nerfstudio (ns-train)", str(probe["nerfstudio_present"]))
        table.add_row("Postshot (postshot-cli)", str(probe["postshot_present"]))
        table.add_row("Local Training Ready", str(probe["ready_for_local_training"]))
        console.print(table)


@app.command()
def mcp(
    allow_path: Optional[Path] = typer.Option(
        None, "--allow-path", help="Confinement root for file operations"
    ),
):
    """Runs the stdio FastMCP server."""
    from .server import mcp as server_mcp

    server_mcp.run()


if __name__ == "__main__":
    app()
