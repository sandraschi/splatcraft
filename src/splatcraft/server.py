"""FastMCP stdio server for splatcraft."""

from __future__ import annotations

import shutil
from pathlib import Path

from fastmcp import FastMCP

from .io import extract_collider, inspect_splat

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
def splat_collider(file_path: str, output_path: str) -> dict:
    """Extracts oriented bounding box and collision extents for game engine use.

    Args:
        file_path: Source splat file.
        output_path: Path where collider JSON/GLB descriptor should be written.
    """
    path = Path(file_path)
    out = Path(output_path)
    world = inspect_splat(path)
    bounds = extract_collider(world, out)
    return bounds.model_dump()


@mcp.tool()
def splat_probe_local_engines() -> dict:
    """Probes the host system (e.g. RTX 4090) for local 3DGS training backends."""
    nerfstudio = shutil.which("ns-train") is not None
    postshot = (
        shutil.which("postshot-cli") is not None
        or Path(r"C:\Program Files\Jawset Postshot\bin\postshot-cli.exe").exists()
    )
    return {
        "nerfstudio_present": nerfstudio,
        "postshot_present": postshot,
        "ready_for_local_training": nerfstudio or postshot,
    }


def main():
    mcp.run()


if __name__ == "__main__":
    main()
