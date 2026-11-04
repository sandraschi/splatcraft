"""Headless recipes for splatcraft (Democraft compat)."""

from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any

from .io import compress_ply_to_spz, create_sample_ply, extract_collider, inspect_splat


def run_recipe_info(out_dir: Path) -> dict[str, Any]:
    """Recipe 1: splat/info-inspect."""
    t0 = time.time()
    out_dir.mkdir(parents=True, exist_ok=True)

    sample_ply = out_dir / "cube_sample.ply"
    create_sample_ply(sample_ply, count=256)

    world = inspect_splat(sample_ply)
    dt = round(time.time() - t0, 4)

    run_json = {
        "recipe": "splat/info-inspect",
        "status": "success",
        "duration_seconds": dt,
        "splat_count": world.splat_count,
        "format": world.format,
        "file_size_bytes": world.file_size_bytes,
        "bbox_min": world.bbox_min,
        "bbox_max": world.bbox_max,
    }

    report_md = f"""# Splatcraft Recipe: Info & Inspection

- **Recipe:** `splat/info-inspect`
- **Duration:** {dt}s
- **Splat Count:** {world.splat_count:,}
- **Format:** {world.format.upper()}
- **File Size:** {world.file_size_bytes:,} bytes
- **Bounds:** {world.bbox_min} to {world.bbox_max}
"""
    (out_dir / "run.json").write_text(json.dumps(run_json, indent=2), encoding="utf-8")
    (out_dir / "info-report.md").write_text(report_md, encoding="utf-8")

    return run_json


def run_recipe_convert(out_dir: Path) -> dict[str, Any]:
    """Recipe 2: splat/convert-spz-compression."""
    t0 = time.time()
    out_dir.mkdir(parents=True, exist_ok=True)

    sample_ply = out_dir / "uncompressed.ply"
    create_sample_ply(sample_ply, count=1024)

    target_spz = out_dir / "compressed.spz"
    res = compress_ply_to_spz(sample_ply, target_spz)
    dt = round(time.time() - t0, 4)

    run_json = {
        "recipe": "splat/convert-spz-compression",
        "status": "success",
        "duration_seconds": dt,
        "original_bytes": res["original_bytes"],
        "compressed_bytes": res["compressed_bytes"],
        "compression_ratio": res["compression_ratio"],
        "splat_count": res["splat_count"],
    }

    report_md = f"""# Splatcraft Recipe: SPZ Compression

- **Recipe:** `splat/convert-spz-compression`
- **Duration:** {dt}s
- **Original Size:** {res["original_bytes"]:,} bytes
- **Compressed Size:** {res["compressed_bytes"]:,} bytes
- **Compression Ratio:** {res["compression_ratio"]}x
- **Splat Count:** {res["splat_count"]:,}
"""
    (out_dir / "run.json").write_text(json.dumps(run_json, indent=2), encoding="utf-8")
    (out_dir / "convert-report.md").write_text(report_md, encoding="utf-8")

    return run_json


def run_recipe_collider(out_dir: Path) -> dict[str, Any]:
    """Recipe 3: splat/collider-extract."""
    t0 = time.time()
    out_dir.mkdir(parents=True, exist_ok=True)

    sample_ply = out_dir / "scene.ply"
    create_sample_ply(sample_ply, count=512)
    world = inspect_splat(sample_ply)

    target_glb = out_dir / "collider.glb"
    bounds = extract_collider(world, target_glb)
    dt = round(time.time() - t0, 4)

    run_json = {
        "recipe": "splat/collider-extract",
        "status": "success",
        "duration_seconds": dt,
        "glb_bytes": target_glb.stat().st_size,
        "volume": bounds.volume,
        "center": bounds.center,
        "extents": bounds.extents,
        "splat_count": bounds.splat_count,
    }

    report_md = f"""# Splatcraft Recipe: Collider Extraction

- **Recipe:** `splat/collider-extract`
- **Duration:** {dt}s
- **GLB Size:** {target_glb.stat().st_size:,} bytes
- **Volume:** {bounds.volume:.2f}
- **Center:** {bounds.center}
- **Extents:** {bounds.extents}
"""
    (out_dir / "run.json").write_text(json.dumps(run_json, indent=2), encoding="utf-8")
    (out_dir / "collider-report.md").write_text(report_md, encoding="utf-8")

    return run_json


RECIPES = {
    "info": run_recipe_info,
    "convert": run_recipe_convert,
    "collider": run_recipe_collider,
}
