"""Tests for splatcraft I/O and models."""

from pathlib import Path

from splatcraft.io import (
    compress_ply_to_spz,
    create_sample_ply,
    extract_collider,
    inspect_splat,
)


def test_create_and_inspect_sample_ply(tmp_path: Path):
    ply_path = tmp_path / "test_sample.ply"
    create_sample_ply(ply_path, count=8)

    assert ply_path.exists()
    world = inspect_splat(ply_path)

    assert world.splat_count == 8
    assert world.format == "ply"
    assert world.file_size_bytes > 0
    assert len(world.bbox_min) == 3


def test_compress_ply_to_spz(tmp_path: Path):
    ply_path = tmp_path / "test_sample.ply"
    create_sample_ply(ply_path, count=64)

    spz_path = tmp_path / "test_sample.spz"
    res = compress_ply_to_spz(ply_path, spz_path)

    assert spz_path.exists()
    assert res["compression_ratio"] >= 1.0
    assert res["splat_count"] == 64

    # Inspect the newly created SPZ
    world = inspect_splat(spz_path)
    assert world.format == "spz"


def test_extract_collider_json(tmp_path: Path):
    ply_path = tmp_path / "test_sample.ply"
    create_sample_ply(ply_path, count=8)
    world = inspect_splat(ply_path)

    out_json = tmp_path / "collider.json"
    bounds = extract_collider(world, out_json)

    assert out_json.exists()
    assert bounds.splat_count == 8
    assert bounds.volume > 0.0


def test_extract_collider_glb(tmp_path: Path):
    ply_path = tmp_path / "test_sample.ply"
    create_sample_ply(ply_path, count=8)
    world = inspect_splat(ply_path)

    out_glb = tmp_path / "collider.glb"
    bounds = extract_collider(world, out_glb)

    assert out_glb.exists()
    assert bounds.volume > 0.0

    # Verify binary GLB header
    with open(out_glb, "rb") as f:
        magic = f.read(4)
        assert magic == b"glTF"
