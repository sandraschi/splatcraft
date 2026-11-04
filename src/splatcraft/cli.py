"""Command line interface for splatcraft."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Optional

import typer
from rich.console import Console
from rich.table import Table

from . import __version__
from .cloud import WorldLabsClient
from .engines import probe_local_engines
from .io import compress_ply_to_spz, extract_collider, inspect_splat
from .recipes import RECIPES
from .train import run_local_training

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
def convert(
    in_file: Path = typer.Option(..., "--in", "-i", help="Source splat file (.ply)"),
    to: str = typer.Option("spz", "--to", "-t", help="Target format (spz)"),
    out: Path = typer.Option(..., "--out", "-o", help="Target output file"),
    as_json: bool = typer.Option(False, "--json", help="Emit JSON output"),
):
    """Converts and compresses Gaussian splat files."""
    try:
        if to.lower() == "spz":
            res = compress_ply_to_spz(in_file, out)
            if as_json:
                console.print(json.dumps(res, indent=2))
            else:
                console.print(
                    f"[bold green]Converted:[/bold green] {in_file} -> {out} "
                    f"(ratio: {res['compression_ratio']}x, {res['compressed_bytes']:,} bytes)"
                )
        else:
            raise ValueError(f"Unsupported target format: {to}")
    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
        raise typer.Exit(code=1)


@app.command()
def collider(
    file: Path = typer.Option(..., "--file", "-f", help="Source splat file"),
    out: Path = typer.Option(..., "--out", "-o", help="Output collider path (.glb or .json)"),
    as_json: bool = typer.Option(False, "--json", help="Emit JSON output"),
):
    """Extracts bounding collider (.glb binary mesh or .json) for gamecraft/Godot integration."""
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
def create(
    prompt: str = typer.Option(..., "--prompt", "-p", help="Text description of the 3D world"),
    name: str = typer.Option("", "--name", help="Scene name"),
    model: str = typer.Option("marble-1.1", "--model", help="World Labs model"),
    out: Optional[Path] = typer.Option(
        None, "--out", "-o", help="Output directory for generated SPZ"
    ),
    dry_run: bool = typer.Option(
        False, "--dry-run", help="Simulate generation without calling API"
    ),
    as_json: bool = typer.Option(False, "--json", help="Emit JSON output"),
):
    """Generates a 3D radiance field world via World Labs Marble API."""
    try:
        client = WorldLabsClient()
        res = client.generate(prompt=prompt, name=name, model=model, dry_run=dry_run, out_dir=out)
        if as_json:
            console.print(res.model_dump_json(indent=2))
        else:
            console.print(
                f"[bold green]World generated ({res.status}):[/bold green] ID={res.world_id} URL={res.spz_url}"
            )
    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
        raise typer.Exit(code=1)


@app.command()
def train(
    input_path: Path = typer.Option(..., "--input", "-i", help="Path to video or image directory"),
    out: Path = typer.Option(..., "--out", "-o", help="Output directory for trained splat"),
    engine: str = typer.Option(
        "nerfstudio", "--engine", "-e", help="Training engine (nerfstudio/postshot)"
    ),
    iterations: int = typer.Option(15000, "--iterations", help="Number of training iterations"),
    dry_run: bool = typer.Option(False, "--dry-run", help="Dry-run command planning for CI"),
    as_json: bool = typer.Option(False, "--json", help="Emit JSON output"),
):
    """Trains a local 3D Gaussian Splat on RTX 4090."""
    try:
        res = run_local_training(
            input_path=input_path,
            out_dir=out,
            engine=engine,
            iterations=iterations,
            dry_run=dry_run,
        )
        if as_json:
            console.print(res.model_dump_json(indent=2))
        else:
            console.print(
                f"[bold green]Training finished ({res.status}):[/bold green] Output: {res.output_ply} ({res.duration_seconds}s)"
            )
    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
        raise typer.Exit(code=1)


@app.command()
def engines(
    as_json: bool = typer.Option(False, "--json", help="Emit JSON status"),
):
    """Probes local host (RTX 4090) for Nerfstudio and Postshot training engines."""
    probe = probe_local_engines()
    if as_json:
        console.print(json.dumps(probe, indent=2))
    else:
        table = Table(title="Local 3DGS Training Engines")
        table.add_column("Engine", style="cyan")
        table.add_column("Available", style="green")
        table.add_column("Path / Binary", style="dim")
        table.add_row(
            "Nerfstudio (ns-train)",
            str(probe["nerfstudio_present"]),
            probe["nerfstudio_path"] or "Not found",
        )
        table.add_row(
            "Postshot (postshot-cli)",
            str(probe["postshot_present"]),
            probe["postshot_path"] or "Not found",
        )
        table.add_row(
            "Local Training Ready",
            str(probe["ready_for_local_training"]),
            "RTX 4090 Capable" if probe["ready_for_local_training"] else "No engines detected",
        )
        console.print(table)


@app.command()
def recipe(
    name: str = typer.Argument("all", help="Recipe name: info, convert, collider, or all"),
    out: Path = typer.Option(Path("./outbox"), "--out", "-o", help="Base outbox directory"),
    as_json: bool = typer.Option(False, "--json", help="Emit JSON output"),
):
    """Runs headless Democraft recipes and generates run.json artifacts."""
    results = {}
    to_run = list(RECIPES.keys()) if name == "all" else [name]

    for rec_name in to_run:
        if rec_name not in RECIPES:
            console.print(
                f"[bold red]Unknown recipe:[/bold red] {rec_name}. Choose from: {list(RECIPES.keys())}"
            )
            raise typer.Exit(code=1)

        rec_dir = out / "splat" / rec_name
        res = RECIPES[rec_name](rec_dir)
        results[rec_name] = res
        if not as_json:
            console.print(
                f"[bold green]Recipe '{rec_name}':[/bold green] Success -> {rec_dir / 'run.json'}"
            )

    if as_json:
        console.print(json.dumps(results, indent=2))


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
