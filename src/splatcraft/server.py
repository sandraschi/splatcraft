"""FastMCP stdio server for splatcraft."""

from __future__ import annotations

from pathlib import Path

from fastmcp import FastMCP

from .cloud import WorldLabsClient
from .engines import probe_local_engines
from .io import compress_ply_to_spz, extract_collider, inspect_splat
from .train import run_local_training

mcp = FastMCP(
    name="splatcraft",
)


@mcp.tool()
def splat_info(file_path: str) -> dict:
    """Inspects a 3D Gaussian Splat file (.ply, .spz) and returns its metadata.

    Args:
        file_path: Path to the splat file.
    """
    path = Path(file_path)
    world = inspect_splat(path)
    return world.model_dump()


@mcp.tool()
def splat_convert(in_path: str, out_path: str, target_format: str = "spz") -> dict:
    """Converts a PLY Gaussian splat file into an SPZ compressed format.

    Args:
        in_path: Source .ply file.
        out_path: Destination .spz file.
        target_format: Target format (default: spz).
    """
    if target_format.lower() == "spz":
        return compress_ply_to_spz(Path(in_path), Path(out_path))
    raise ValueError(f"Unsupported format: {target_format}")


@mcp.tool()
def splat_collider(file_path: str, output_path: str) -> dict:
    """Extracts oriented bounding box and collision mesh (.glb / .json) for gamecraft/Godot.

    Args:
        file_path: Source splat file.
        output_path: Path where collider GLB or JSON should be written.
    """
    path = Path(file_path)
    out = Path(output_path)
    world = inspect_splat(path)
    bounds = extract_collider(world, out)
    return bounds.model_dump()


@mcp.tool()
def splat_create_world(
    prompt: str, name: str = "", model: str = "marble-1.1", dry_run: bool = False
) -> dict:
    """Generates a 3D radiance field world via World Labs Marble API.

    Args:
        prompt: Natural language description of the world.
        name: Optional scene name.
        model: World Labs model tier (default: marble-1.1).
        dry_run: Simulate generation without consuming API credits.
    """
    client = WorldLabsClient()
    res = client.generate(prompt=prompt, name=name, model=model, dry_run=dry_run)
    return res.model_dump()


@mcp.tool()
def splat_train_local(
    input_path: str,
    output_dir: str,
    engine: str = "nerfstudio",
    iterations: int = 15000,
    dry_run: bool = False,
) -> dict:
    """Trains a 3D Gaussian Splat locally on RTX 4090 using Nerfstudio or Postshot.

    Args:
        input_path: Path to video or directory of images.
        output_dir: Target export directory for trained splat.ply.
        engine: "nerfstudio" or "postshot".
        iterations: Number of training iterations.
        dry_run: Plan execution commands without running heavy GPU steps.
    """
    res = run_local_training(
        input_path=Path(input_path),
        out_dir=Path(output_dir),
        engine=engine,
        iterations=iterations,
        dry_run=dry_run,
    )
    return res.model_dump()


@mcp.tool()
def splat_probe_local_engines() -> dict:
    """Probes the host system (e.g. RTX 4090) for local 3DGS training backends."""
    return probe_local_engines()


def main():
    mcp.run()


if __name__ == "__main__":
    main()
