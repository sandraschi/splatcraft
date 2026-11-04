"""I/O routines for 3D Gaussian Splats (PLY, SPZ, colliders)."""

from __future__ import annotations

from pathlib import Path

import numpy as np

from .models import ColliderBounds, SplatWorld


def parse_ply_header(path: Path) -> dict:
    """Fast header parser for PLY files without reading full binary payload."""
    splat_count = 0
    sh_degree = 0
    properties: list[str] = []

    with open(path, "rb") as f:
        line = f.readline().decode("ascii", errors="ignore").strip()
        if line != "ply":
            raise ValueError(f"Not a valid PLY file: {path}")

        for raw_line in f:
            line_str = raw_line.decode("ascii", errors="ignore").strip()
            if line_str.startswith("element vertex"):
                parts = line_str.split()
                if len(parts) >= 3:
                    splat_count = int(parts[2])
            elif line_str.startswith("property"):
                parts = line_str.split()
                if len(parts) >= 3:
                    properties.append(parts[2])
            elif line_str == "end_header":
                break

    # Determine SH degree based on presence of f_rest coefficients
    rest_coeffs = [p for p in properties if p.startswith("f_rest_")]
    if len(rest_coeffs) >= 45:
        sh_degree = 3
    elif len(rest_coeffs) >= 24:
        sh_degree = 2
    elif len(rest_coeffs) >= 9:
        sh_degree = 1
    else:
        sh_degree = 0

    return {
        "splat_count": splat_count,
        "sh_degree": sh_degree,
        "properties": properties,
    }


def inspect_splat(path: Path) -> SplatWorld:
    """Inspects a splat file and returns its structured metadata."""
    if not path.exists():
        raise FileNotFoundError(f"File not found: {path}")

    ext = path.suffix.lower().lstrip(".")
    if ext not in ("ply", "spz", "rad"):
        ext = "ply"

    file_size = path.stat().st_size

    if ext == "ply":
        header = parse_ply_header(path)
        splat_count = header["splat_count"]
        sh_degree = header["sh_degree"]
    else:
        splat_count = 1000
        sh_degree = 3

    # Default unit bounding box if not parsed from vertices
    bbox_min = [-1.0, -1.0, -1.0]
    bbox_max = [1.0, 1.0, 1.0]

    return SplatWorld(
        id=path.stem,
        name=path.name,
        file_path=str(path.resolve()),
        format=ext,  # type: ignore
        splat_count=splat_count,
        sh_degree=sh_degree,
        bbox_min=bbox_min,
        bbox_max=bbox_max,
        file_size_bytes=file_size,
        provider="imported",
    )


def extract_collider(splat: SplatWorld, out_path: Path) -> ColliderBounds:
    """Extracts bounding box collider data and writes a collider sidecar descriptor."""
    min_b = np.array(splat.bbox_min)
    max_b = np.array(splat.bbox_max)
    extents = (max_b - min_b).tolist()
    center = ((min_b + max_b) / 2.0).tolist()
    volume = float(np.prod(extents))

    bounds = ColliderBounds(
        center=center,
        extents=extents,
        volume=volume,
        splat_count=splat.splat_count,
        source_file=splat.file_path,
    )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(bounds.model_dump_json(indent=2))

    return bounds


def create_sample_ply(path: Path, count: int = 8) -> Path:
    """Generates a minimal valid Gaussian Splat PLY file for testing."""
    path.parent.mkdir(parents=True, exist_ok=True)
    header = f"""ply
format ascii 1.0
element vertex {count}
property float x
property float y
property float z
property float f_dc_0
property float f_dc_1
property float f_dc_2
property float opacity
property float scale_0
property float scale_1
property float scale_2
property float rot_0
property float rot_1
property float rot_2
property float rot_3
end_header
"""
    with open(path, "w", encoding="ascii") as f:
        f.write(header)
        for i in range(count):
            x = (i % 2) * 2.0 - 1.0
            y = ((i // 2) % 2) * 2.0 - 1.0
            z = ((i // 4) % 2) * 2.0 - 1.0
            f.write(f"{x} {y} {z} 0.5 0.5 0.5 1.0 0.1 0.1 0.1 1.0 0.0 0.0 0.0\n")

    return path
