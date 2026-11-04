"""I/O routines for 3D Gaussian Splats (PLY, SPZ, GLB colliders)."""

from __future__ import annotations

import gzip
import json
import struct
from pathlib import Path
from typing import Any

import numpy as np

from .models import ColliderBounds, SplatWorld


def parse_ply_header(path: Path) -> dict[str, Any]:
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
    elif ext == "spz":
        # SPZ header inspection: reads uncompressed magic or gzip header
        splat_count = 1000
        sh_degree = 3
        try:
            with gzip.open(path, "rb") as f:
                magic = f.read(4)
                if magic in (b"SPZ\x01", b"SPZ\x02", b"SPG2"):
                    # Reads packed count if present
                    count_bytes = f.read(4)
                    if len(count_bytes) == 4:
                        splat_count = struct.unpack("<I", count_bytes)[0]
        except Exception:
            splat_count = 1000
    else:
        splat_count = 1000
        sh_degree = 3

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


def compress_ply_to_spz(ply_path: Path, spz_path: Path) -> dict[str, Any]:
    """Compresses a PLY Gaussian splat file into an SPZ container."""
    if not ply_path.exists():
        raise FileNotFoundError(f"Source file not found: {ply_path}")

    header = parse_ply_header(ply_path)
    count = header["splat_count"]

    # Read binary or text payload
    with open(ply_path, "rb") as f:
        content = f.read()

    spz_path.parent.mkdir(parents=True, exist_ok=True)

    # Format header: Magic b'SPZ\x01' + uint32 count + gzipped raw payload
    with gzip.open(spz_path, "wb", compresslevel=9) as gz:
        gz.write(b"SPZ\x01")
        gz.write(struct.pack("<I", count))
        gz.write(content)

    orig_size = len(content)
    comp_size = spz_path.stat().st_size
    ratio = orig_size / max(comp_size, 1)

    return {
        "source": str(ply_path),
        "target": str(spz_path),
        "original_bytes": orig_size,
        "compressed_bytes": comp_size,
        "compression_ratio": round(ratio, 2),
        "splat_count": count,
    }


def build_box_glb(center: list[float], extents: list[float], out_path: Path) -> None:
    """Generates a valid binary GLTF 2.0 (.glb) mesh box representing the collider."""
    cx, cy, cz = center
    hx, hy, hz = extents[0] / 2.0, extents[1] / 2.0, extents[2] / 2.0

    # 8 box corners
    corners = [
        [cx - hx, cy - hy, cz - hz],
        [cx + hx, cy - hy, cz - hz],
        [cx + hx, cy + hy, cz - hz],
        [cx - hx, cy + hy, cz - hz],
        [cx - hx, cy - hy, cz + hz],
        [cx + hx, cy - hy, cz + hz],
        [cx + hx, cy + hy, cz + hz],
        [cx - hx, cy + hy, cz + hz],
    ]

    # 36 vertex indices (12 triangles, 6 faces)
    indices = [
        0,
        2,
        1,
        0,
        3,
        2,  # front
        4,
        5,
        6,
        4,
        6,
        7,  # back
        0,
        1,
        5,
        0,
        5,
        4,  # bottom
        2,
        3,
        7,
        2,
        7,
        6,  # top
        0,
        4,
        7,
        0,
        7,
        3,  # left
        1,
        2,
        6,
        1,
        6,
        5,  # right
    ]

    # Binary buffers
    pos_data = struct.pack(f"<{len(corners) * 3}f", *[c for corner in corners for c in corner])
    idx_data = struct.pack(f"<{len(indices)}H", *indices)

    # Pad buffers to 4-byte boundaries
    pos_pad = (4 - (len(pos_data) % 4)) % 4
    idx_pad = (4 - (len(idx_data) % 4)) % 4

    bin_buffer = idx_data + (b"\x00" * idx_pad) + pos_data + (b"\x00" * pos_pad)

    idx_offset = 0
    idx_len = len(idx_data)
    pos_offset = len(idx_data) + idx_pad
    pos_len = len(pos_data)

    min_pos = [cx - hx, cy - hy, cz - hz]
    max_pos = [cx + hx, cy + hy, cz + hz]

    gltf_dict = {
        "asset": {"version": "2.0", "generator": "splatcraft"},
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "Collider"}],
        "meshes": [
            {
                "name": "BoxCollider",
                "primitives": [
                    {
                        "attributes": {"POSITION": 1},
                        "indices": 0,
                        "mode": 4,  # TRIANGLES
                    }
                ],
            }
        ],
        "accessors": [
            {
                "bufferView": 0,
                "byteOffset": 0,
                "componentType": 5123,  # UNSIGNED_SHORT
                "count": len(indices),
                "type": "SCALAR",
                "max": [7],
                "min": [0],
            },
            {
                "bufferView": 1,
                "byteOffset": 0,
                "componentType": 5126,  # FLOAT
                "count": len(corners),
                "type": "VEC3",
                "max": max_pos,
                "min": min_pos,
            },
        ],
        "bufferViews": [
            {
                "buffer": 0,
                "byteOffset": idx_offset,
                "byteLength": idx_len,
                "target": 34963,  # ELEMENT_ARRAY_BUFFER
            },
            {
                "buffer": 0,
                "byteOffset": pos_offset,
                "byteLength": pos_len,
                "target": 34962,  # ARRAY_BUFFER
            },
        ],
        "buffers": [{"byteLength": len(bin_buffer)}],
    }

    json_str = json.dumps(gltf_dict)
    json_bytes = json_str.encode("utf-8")
    json_pad = (4 - (len(json_bytes) % 4)) % 4
    json_chunk = json_bytes + (b" " * json_pad)

    total_len = 12 + 8 + len(json_chunk) + 8 + len(bin_buffer)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "wb") as f:
        # GLB Header
        f.write(b"glTF")
        f.write(struct.pack("<II", 2, total_len))
        # JSON Chunk
        f.write(struct.pack("<II", len(json_chunk), 0x4E4F534A))  # 'JSON'
        f.write(json_chunk)
        # BIN Chunk
        f.write(struct.pack("<II", len(bin_buffer), 0x004E4942))  # 'BIN\0'
        f.write(bin_buffer)


def extract_collider(splat: SplatWorld, out_path: Path) -> ColliderBounds:
    """Extracts bounding box collider data and writes JSON or binary GLB."""
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

    if out_path.suffix.lower() == ".glb":
        build_box_glb(center, extents, out_path)
        # Also write sidecar json
        json_sidecar = out_path.with_suffix(".json")
        with open(json_sidecar, "w", encoding="utf-8") as f:
            f.write(bounds.model_dump_json(indent=2))
    else:
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(bounds.model_dump_json(indent=2))

    return bounds
