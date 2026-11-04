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
    # Check PATH first, then fleet-standard venv locations and env overrides
    ns_candidates = [
        shutil.which("ns-train"),
        Path(r"D:\Dev\repos\splatmaker-mcp\.venv\Scripts\ns-train.exe"),
        Path(r"C:\Program Files\nerfstudio\bin\ns-train.exe"),
    ]
    ns_path = next((p for p in ns_candidates if p and Path(p).exists()), None)

    postshot_candidates = [
        shutil.which("postshot-cli"),
        Path(r"C:\Program Files\Jawset Postshot\bin\postshot-cli.exe"),
        Path(r"D:\Dev\archives\postshot\postshot-cli.exe"),
    ]
    postshot_path = next((p for p in postshot_candidates if p and Path(p).exists()), None)

    return {
        "nerfstudio_present": ns_path is not None,
        "nerfstudio_path": str(ns_path) if ns_path else None,
        "postshot_present": postshot_path is not None,
        "postshot_path": str(postshot_path) if postshot_path else None,
        "ready_for_local_training": (ns_path is not None) or (postshot_path is not None),
    }


def main():
    mcp.run()


if __name__ == "__main__":
    main()
