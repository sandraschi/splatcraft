"""Tests for splatcraft I/O and models."""

from pathlib import Path

from splatcraft.io import create_sample_ply, extract_collider, inspect_splat


def test_create_and_inspect_sample_ply(tmp_path: Path):
    ply_path = tmp_path / "test_sample.ply"
    create_sample_ply(ply_path, count=8)

    assert ply_path.exists()
    world = inspect_splat(ply_path)

    assert world.splat_count == 8
    assert world.format == "ply"
    assert world.file_size_bytes > 0
    assert len(world.bbox_min) == 3


def test_extract_collider(tmp_path: Path):
    ply_path = tmp_path / "test_sample.ply"
    create_sample_ply(ply_path, count=8)
    world = inspect_splat(ply_path)

    out_json = tmp_path / "collider.json"
    bounds = extract_collider(world, out_json)

    assert out_json.exists()
    assert bounds.splat_count == 8
    assert bounds.volume > 0.0
